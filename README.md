# iso2jpg

Extract image files from ISO 9660 disk images without mounting the ISO.

Rust CLI tool for old photo/image discs and other ISO
containers. It scans the filesystem inside a `.iso`, finds files with matching
extensions, copies them to a normal folder, and uses ImageMagick to convert extracted
non-JPEG images to `.jpg`.

Sample ISO files available at [archive.org](https://archive.org/details/Corel_Professional_Photos_Collection_1994).

Sample converted JPGs on [are.na](https://www.are.na/gregory-cotton/corel-professional-photos-collection).

## Requirements

- Rust and Cargo
- ImageMagick

On macOS (with Homebrew):

```bash
brew install imagemagick
```

## Using it

Make sure all requirements are installed.

To extract `.PCD` files and convert them to `.jpg` in one run:

```bash
cargo run -- "/Users/username/Downloads/Corel Professional Photos - 1994 - 001 - Sunrises and Sunsets.ISO" \
  --out ./extracted \
  --extensions pcd \
  --convert-to-jpg \
  --manifest ./manifest.json
```
(Replace the file path with your `.iso` file path)

If output files already exist and you want to replace them, add:

```bash
--overwrite
```

To view other options:

```bash
cargo run -- --help
```