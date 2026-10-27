"""Compress reviewed 1080x1440 poster captures and update the shared catalog.
Usage: python3 scripts/compress-share-images.py artifacts/product-share-source
Requires Pillow only for artwork authoring, not for running or building Starclip.
"""
import argparse
import io
import json
from pathlib import Path
from PIL import Image

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('source', type=Path)
args = parser.parse_args()
root = Path(__file__).resolve().parent.parent
catalog_path = root / 'src/product-share-catalog.json'
catalog = json.loads(catalog_path.read_text())
outputs = []
for item in catalog['images']:
    with Image.open(args.source / f"{item['id']}.png") as original:
        expected = (item['width'], item['height'])
        if original.size != expected:
            raise ValueError(f"{item['id']}: expected {expected}, got {original.size}")
        # Indexed PNG keeps type/UI edges crisp while reducing the bundled payload.
        compressed = original.convert('RGB').quantize(colors=256, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE)
        buffer = io.BytesIO()
        compressed.save(buffer, format='PNG', optimize=True)
        data = buffer.getvalue()
        if len(data) > 300 * 1024:
            raise ValueError(f"{item['id']}: exceeds 300 KiB artwork budget")
        item['bytes'] = len(data)
        outputs.append((root / 'public' / item['path'], data))
for path, data in outputs:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    print(f'{path.name}: {len(data):,} bytes')
catalog_path.write_text(json.dumps(catalog, ensure_ascii=False, indent=2) + '\n')
print(f'Total: {sum(len(data) for _, data in outputs):,} bytes')
