//! Optional local integration with the standalone Xingjian renderer.
//! No text is placed in command arguments or kept after a render.
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    fs::{self, DirBuilder, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use crate::store::Result;
const MAX_TEXT: usize = 10_000;
const MAX_PNG: u64 = 10 * 1024 * 1024;
const MAX_PIXELS: u64 = 8_000_000;
static CLI_GATE: Mutex<()> = Mutex::new(());
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HandwritingAsset {
    pub name: String,
    pub path: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HandwritingAssets {
    pub available: bool,
    pub path: Option<String>,
    pub fonts: Vec<HandwritingAsset>,
    pub backgrounds: Vec<HandwritingAsset>,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HandwritingOptions {
    pub text: String,
    pub font: String,
    pub background: String,
    pub font_size: u32,
    pub ink: String,
    pub padding: u32,
}
#[derive(Deserialize)]
struct CliAssets {
    fonts: Vec<String>,
    backgrounds: Vec<String>,
}
#[derive(Deserialize, Serialize)]
struct AppSelection {
    path: String,
}

fn unavailable(error: impl Into<String>) -> HandwritingAssets {
    HandwritingAssets {
        available: false,
        path: None,
        fonts: vec![],
        backgrounds: vec![],
        error: Some(error.into()),
    }
}
fn config_path(directory: &Path) -> PathBuf {
    directory.join("handwriting-app.json")
}
fn home_directory() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}
fn executable_name() -> &'static str {
    if cfg!(windows) {
        "xingjian.exe"
    } else {
        "xingjian"
    }
}
fn normalize_app(path: &Path) -> Result<PathBuf> {
    let path = if path.is_dir() {
        if path.extension().is_some_and(|ext| ext == "app") {
            path.join("Contents/MacOS/xingjian")
        } else {
            path.join(executable_name())
        }
    } else {
        path.to_owned()
    };
    let path = path
        .canonicalize()
        .map_err(|_| "未找到可运行的星笺程序，请选择星笺.app 或 xingjian CLI。")?;
    if !path.is_file() {
        return Err("请选择星笺应用或 CLI 文件。".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if path
            .metadata()
            .map_err(|_| "无法读取程序信息。")?
            .permissions()
            .mode()
            & 0o111
            == 0
        {
            return Err("所选文件不可执行，请选择已安装的星笺程序。".into());
        }
    }
    Ok(path)
}
fn automatic_candidates() -> Vec<PathBuf> {
    let mut paths = vec![];
    if let Some(home) = home_directory() {
        if cfg!(target_os = "macos") {
            paths.push(home.join("Applications/星笺.app"));
            paths.push(PathBuf::from("/Applications/星笺.app"));
            paths.push(home.join("git_pro/HandwritingGenerator/outputs/rust-build/星笺.app"));
        } else {
            paths.push(home.join("Applications").join(executable_name()));
            paths.push(
                home.join("git_pro/HandwritingGenerator/outputs/rust-build")
                    .join(executable_name()),
            );
        }
        paths.push(
            home.join("git_pro/HandwritingGenerator/rust/target/release")
                .join(executable_name()),
        );
    }
    paths
}
fn discover(directory: &Path) -> Result<PathBuf> {
    let selected = config_path(directory);
    if selected.exists() {
        let file =
            File::open(selected).map_err(|_| "无法读取手写应用设置，请重新选择星笺应用。")?;
        let selection: AppSelection = serde_json::from_reader(file.take(16 * 1024))
            .map_err(|_| "手写应用设置无效，请重新选择星笺应用。")?;
        return normalize_app(&crate::storage::renderer_path(
            directory,
            Path::new(&selection.path),
        )?);
    }
    automatic_candidates().iter().find_map(|path| normalize_app(path).ok())
        .ok_or_else(|| "未发现本机星笺手写应用。请先安装星笺，再点击「选择星笺应用」选择 .app 或 xingjian CLI。".into())
}

/// An exclusively created private directory also owns failed/partial renderer output.
struct TemporaryDirectory {
    path: PathBuf,
}
impl TemporaryDirectory {
    fn in_directory(parent: &Path) -> Result<Self> {
        for _ in 0..100 {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(
                "starclip-handwriting-{}-{sequence}",
                std::process::id()
            ));
            let mut builder = DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => return Err("无法创建渲染临时目录，请检查磁盘空间。".into()),
            }
        }
        Err("无法创建独占临时目录，请稍后重试。".into())
    }
    fn new() -> Result<Self> {
        Self::in_directory(&std::env::temp_dir())
    }
}
impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
struct RunningChild(Child);
impl Drop for RunningChild {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}
fn read_bounded(mut stream: impl Read, limit: u64) -> std::io::Result<Vec<u8>> {
    let mut bytes = vec![];
    stream.by_ref().take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(std::io::Error::other("CLI output exceeds limit"));
    }
    Ok(bytes)
}
fn run_cli(
    executable: &Path,
    args: &[OsString],
    text: Option<&str>,
    timeout: Duration,
) -> Result<Vec<u8>> {
    let mut command = Command::new(executable);
    command
        .args(args)
        .stdin(if text.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = RunningChild(
        command
            .spawn()
            .map_err(|_| "无法启动星笺 CLI，请检查所选应用。")?,
    );
    let stdout = child.0.stdout.take().ok_or("无法读取星笺响应。")?;
    let stderr = child.0.stderr.take().ok_or("无法读取星笺状态。")?;
    let output = thread::spawn(move || read_bounded(stdout, 128 * 1024));
    let errors = thread::spawn(move || read_bounded(stderr, 8 * 1024));
    let input = text.map(|text| {
        let mut stdin = child.0.stdin.take().expect("piped stdin");
        let text = text.as_bytes().to_vec();
        thread::spawn(move || stdin.write_all(&text))
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.0.try_wait().map_err(|_| "无法读取渲染进程状态。")? {
            break status;
        }
        if started.elapsed() >= timeout {
            let _ = child.0.kill();
            let _ = child.0.wait();
            return Err("星笺渲染超时，已停止本次任务。请缩短正文或降低字号后重试。".into());
        }
        thread::sleep(Duration::from_millis(20));
    };
    if let Some(input) = input {
        input
            .join()
            .map_err(|_| "正文传输失败。")?
            .map_err(|_| "星笺未能读取正文。")?;
    }
    let output = output
        .join()
        .map_err(|_| "星笺响应读取失败。")?
        .map_err(|_| "星笺响应过大或读取失败。")?;
    let _ = errors.join();
    if !status.success() {
        return Err("星笺未能完成任务，请检查应用版本、字体和纸张设置。".into());
    }
    Ok(output)
}
fn valid_builtin(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(|name| {
        !name.is_empty()
            && name.len() <= 240
            && !name.contains(['/', '\\', '\0'])
            && name != "."
            && name != ".."
    })
}
fn load_assets(executable: &Path) -> Result<HandwritingAssets> {
    let output = run_cli(executable, &["assets".into()], None, Duration::from_secs(5))?;
    let assets: CliAssets = serde_json::from_slice(&output)
        .map_err(|_| "该程序不支持星笺 assets 接口，请选择新版星笺 CLI。")?;
    if assets.fonts.is_empty()
        || assets.fonts.len() > 100
        || assets.backgrounds.len() > 100
        || assets
            .fonts
            .iter()
            .any(|value| !valid_builtin(value, "builtin:fonts/"))
        || assets
            .backgrounds
            .iter()
            .any(|value| !valid_builtin(value, "builtin:bgs/"))
    {
        return Err("星笺返回了不支持的素材清单。".into());
    }
    let label = |path: String| HandwritingAsset {
        name: Path::new(&path)
            .file_stem()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone()),
        path,
    };
    Ok(HandwritingAssets {
        available: true,
        path: Some(executable.to_string_lossy().into_owned()),
        fonts: assets.fonts.into_iter().map(label).collect(),
        backgrounds: assets.backgrounds.into_iter().map(label).collect(),
        error: None,
    })
}
pub fn assets(directory: &Path) -> HandwritingAssets {
    let Ok(_guard) = CLI_GATE.try_lock() else {
        return unavailable("手写渲染正在进行，请完成后重试。");
    };
    discover(directory)
        .and_then(|executable| load_assets(&executable))
        .unwrap_or_else(unavailable)
}
pub fn select_app(directory: &Path, selected: &Path) -> Result<HandwritingAssets> {
    let _guard = CLI_GATE
        .try_lock()
        .map_err(|_| "手写渲染正在进行，请稍后再选择应用。")?;
    let executable = normalize_app(selected)?;
    let assets = load_assets(&executable)?;
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(directory)
        .map_err(|_| "无法保存手写应用位置。")?;
    let scratch = TemporaryDirectory::in_directory(directory)?;
    let temporary = scratch.path.join("selection.json");
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|_| "无法保存手写应用位置。")?;
    let portable = executable
        .strip_prefix(directory)
        .ok()
        .filter(|path| path.starts_with("renderers"));
    let path = match portable {
        Some(relative) => crate::storage::portable_renderer_path(relative)?,
        None => executable
            .to_str()
            .ok_or("手写应用路径不是有效的 Unicode。")?
            .to_owned(),
    };
    let bytes = serde_json::to_vec(&AppSelection { path }).map_err(|_| "无法保存手写应用位置。")?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| "无法保存手写应用位置，请检查磁盘空间。")?;
    drop(file);
    fs::rename(temporary, config_path(directory)).map_err(|_| "无法保存手写应用位置。")?;
    Ok(assets)
}
fn is_color(color: &str) -> bool {
    color.len() == 7
        && color.starts_with('#')
        && color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
}
fn validate_dimensions(width: u32, height: u32) -> Result<(u32, u32)> {
    if !(128..=4096).contains(&width)
        || !(128..=4096).contains(&height)
        || u64::from(width) * u64::from(height) > MAX_PIXELS
    {
        return Err("纸张尺寸需为 128–4096 像素，且总像素不超过 800 万。".into());
    }
    Ok((width, height))
}
fn solid_dimensions(background: &str) -> Result<(u32, u32)> {
    let parts: Vec<_> = background.split(':').collect();
    if parts.len() != 4 || parts[0] != "solid" || !is_color(parts[1]) {
        return Err("纯色纸张格式无效。".into());
    }
    let width = parts[2].parse().map_err(|_| "纸张宽度无效。")?;
    let height = parts[3].parse().map_err(|_| "纸张高度无效。")?;
    validate_dimensions(width, height)
}
fn validate_options(options: &HandwritingOptions, assets: &HandwritingAssets) -> Result<()> {
    if options.text.trim().is_empty()
        || options.text.chars().count() > MAX_TEXT
        || options.text.contains('\0')
    {
        return Err("正文需为 1–10000 个字符，且不能包含空字符。".into());
    }
    if !(8..=120).contains(&options.font_size) || options.padding > 240 || !is_color(&options.ink) {
        return Err("字号需为 8–120，边距为 0–240，墨色为 #RRGGBB。".into());
    }
    if !assets.fonts.iter().any(|font| font.path == options.font) {
        return Err("请选择星笺内置字体。".into());
    }
    if options.background.starts_with("solid:") {
        solid_dimensions(&options.background)?;
    } else if !assets
        .backgrounds
        .iter()
        .any(|background| background.path == options.background)
    {
        return Err("请选择星笺内置纸张或纯色纸张。".into());
    }
    Ok(())
}
fn png_dimensions(bytes: &[u8]) -> Result<(u32, u32)> {
    if bytes.len() < 33 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") || &bytes[12..16] != b"IHDR" {
        return Err("星笺未返回有效的 PNG 图片。".into());
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
    validate_dimensions(width, height)
}
fn read_png(path: &Path) -> Result<Vec<u8>> {
    let file = File::open(path).map_err(|_| "星笺没有生成图片。")?;
    let bytes = read_bounded(file, MAX_PNG).map_err(|_| "生成图片超过 10 MB 或无法读取。")?;
    png_dimensions(&bytes)?;
    Ok(bytes)
}
fn render_arguments(
    options: &HandwritingOptions,
    output: &Path,
    width: u32,
    height: u32,
    padding: u32,
) -> Vec<OsString> {
    let mut args: Vec<OsString> = [
        "render",
        "--text-file",
        "-",
        "--font",
        &options.font,
        "--background",
        &options.background,
        "--color",
        &options.ink,
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    for (flag, value) in [
        ("--font-size", options.font_size),
        ("--x", padding),
        ("--y", padding),
        ("--width", width),
        ("--height", height),
    ] {
        args.push(flag.into());
        args.push(value.to_string().into());
    }
    args.push("--output".into());
    args.push(output.as_os_str().to_owned());
    args
}
fn render_with(
    executable: &Path,
    options: &HandwritingOptions,
    assets: &HandwritingAssets,
) -> Result<String> {
    validate_options(options, assets)?;
    let scratch = TemporaryDirectory::new()?;
    let dimensions = if options.background.starts_with("solid:") {
        solid_dimensions(&options.background)?
    } else {
        // The current CLI asset schema has no dimensions. Probe the exact built-in paper without text.
        let paper = scratch.path.join("paper.png");
        run_cli(
            executable,
            &render_arguments(options, &paper, 1, 1, 0),
            Some(""),
            Duration::from_secs(15),
        )?;
        png_dimensions(&read_png(&paper)?)?
    };
    let width = dimensions
        .0
        .checked_sub(options.padding * 2)
        .ok_or("边距超出纸张宽度。")?;
    let height = dimensions
        .1
        .checked_sub(options.padding * 2)
        .ok_or("边距超出纸张高度。")?;
    if width < options.font_size * 2 || height < options.font_size * 2 {
        return Err("纸张可书写区域过小，请减小边距或字号。".into());
    }
    let output = scratch.path.join("handwriting.png");
    run_cli(
        executable,
        &render_arguments(options, &output, width, height, options.padding),
        Some(&options.text),
        Duration::from_secs(25),
    )?;
    let png = read_png(&output)?;
    if png_dimensions(&png)? != dimensions {
        return Err("星笺返回的图片尺寸与所选纸张不一致。".into());
    }
    Ok(base64::engine::general_purpose::STANDARD.encode(png))
}
pub fn render(directory: &Path, options: HandwritingOptions) -> Result<String> {
    let _guard = CLI_GATE
        .try_lock()
        .map_err(|_| "已有手写渲染任务，请等待完成后重试。")?;
    let executable = discover(directory)?;
    let assets = load_assets(&executable)?;
    render_with(&executable, &options, &assets)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn options() -> HandwritingOptions {
        HandwritingOptions {
            text: "星光落在纸上。\n每个想法都值得被记住。".into(),
            font: "builtin:fonts/hand.ttf".into(),
            background: "solid:#f8f1e3:1200:1600".into(),
            font_size: 32,
            ink: "#26344a".into(),
            padding: 80,
        }
    }
    fn test_assets() -> HandwritingAssets {
        HandwritingAssets {
            available: true,
            path: None,
            fonts: vec![HandwritingAsset {
                name: "hand".into(),
                path: "builtin:fonts/hand.ttf".into(),
            }],
            backgrounds: vec![HandwritingAsset {
                name: "small".into(),
                path: "builtin:bgs/#000000-200x200.png".into(),
            }],
            error: None,
        }
    }
    #[test]
    fn validates_limits_and_keeps_literal_text_out_of_arguments() {
        let mut options = options();
        let assets = test_assets();
        assert!(validate_options(&options, &assets).is_ok());
        assert_eq!(solid_dimensions(&options.background).unwrap(), (1200, 1600));
        let args = render_arguments(&options, Path::new("output.png"), 1040, 1440, 80);
        assert!(!args
            .iter()
            .any(|arg| arg.to_string_lossy().contains("星光")));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--text-file" && pair[1] == "-"));
        options.text = "星".repeat(MAX_TEXT + 1);
        assert!(validate_options(&options, &assets).is_err());
        options.text = "正文".into();
        options.font = "/private/font.ttf".into();
        assert!(validate_options(&options, &assets).is_err());
        options.font = "builtin:fonts/hand.ttf".into();
        options.background = "solid:#ffffff:90000:90000".into();
        assert!(validate_options(&options, &assets).is_err());
        assert!(!valid_builtin(
            "builtin:fonts/../../secret",
            "builtin:fonts/"
        ));
        assert!(!is_color("#星星"));
    }
    #[test]
    fn failed_process_and_partial_output_are_cleaned() {
        let scratch = TemporaryDirectory::new().unwrap();
        let path = scratch.path.clone();
        fs::write(path.join("partial.png"), b"partial").unwrap();
        assert!(run_cli(
            &path.join("missing-cli"),
            &["assets".into()],
            None,
            Duration::from_millis(20)
        )
        .is_err());
        drop(scratch);
        assert!(!path.exists());
        assert!(png_dimensions(b"not a PNG").is_err());
    }
    #[cfg(unix)]
    #[test]
    fn timed_out_child_is_stopped() {
        let started = Instant::now();
        let error = run_cli(
            Path::new("/bin/sleep"),
            &["5".into()],
            None,
            Duration::from_millis(40),
        )
        .unwrap_err();
        assert!(error.contains("超时"));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    #[ignore = "Requires an installed local Xingjian CLI; uses only original test text"]
    fn local_cli_renders_original_text_to_png() {
        let executable = automatic_candidates()
            .iter()
            .find_map(|path| normalize_app(path).ok())
            .expect("Install the local Xingjian CLI first");
        let assets = load_assets(&executable).unwrap();
        let image = render_with(&executable, &options(), &assets).unwrap();
        let png = base64::engine::general_purpose::STANDARD
            .decode(image)
            .unwrap();
        assert_eq!(png_dimensions(&png).unwrap(), (1200, 1600));
        assert!(png.len() > 1000);
        let mut small = options();
        small.background = "builtin:bgs/#000000-200x200.png".into();
        small.font_size = 16;
        small.padding = 20;
        let png = base64::engine::general_purpose::STANDARD
            .decode(render_with(&executable, &small, &assets).unwrap())
            .unwrap();
        assert_eq!(png_dimensions(&png).unwrap(), (200, 200));
    }
}
