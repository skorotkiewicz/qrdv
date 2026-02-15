# qrdv

Encode any file into a QR code video. Decode it back — even after compression.

## Usage

```bash
# encode
qrdv encode -i backup.tar -o backup.mp4 --key "secret"

# decode
qrdv decode -i backup.mp4 -o backup.tar --key "secret"
```

## Options

| Flag | Description | Default |
|------|-------------|---------|
| `-i` | Input file | — |
| `-o` | Output file | — |
| `-r` | Resolution (`480p` `720p` `1080p` `1440p` `4k`) | `720p` |
| `-k` | Encryption key (optional) | none |
| `-e` | Error correction (`low` `medium` `quartile` `high`) | `high` |
| `--fps` | Frames per second | `2` |

Higher error correction = survives heavier compression, but produces longer videos.

## How It Works

```
file → compress → encrypt → split into chunks → QR frames → MP4
MP4 → extract frames → decode QR → reassemble → decrypt → decompress → file
```

- **Compression**: DEFLATE before encoding to minimize frame count
- **Encryption**: AES-256-GCM with Argon2id key derivation
- **Integrity**: CRC32 per chunk + full file checksum verification
- **Protocol**: Binary header frame with metadata, followed by indexed data frames

## Requirements

- [ffmpeg](https://ffmpeg.org/) in `$PATH`

## Build

```bash
cargo build --release
```

## License

MIT
