# Chess Player Filter

A lightweight desktop tool built in Rust using egui that analyzes Chess.com accounts based on public API data.

## Version

v0.1.0 - initial release

## Features

- Filter players by account age
- Filter by number of games played
- Displays rating 
- Batch username checking (comma-separated input)
- Fast concurrent API requests

## Notes

- This tool uses the official Chess.com public API
- No login or API key required
- All data shown is publicly available

## Technical Details

- Built with Rust
- UI: egui (eframe)
- Async runtime: tokio
- HTTP client: reqwest

## Platform

- Windows (tested)
- Other platforms may require building from source

## Disclaimer

This tool is not affiliated with Chess.com.

## How to run

Download and extract the ZIP, then run the `.exe` file.

## Limitations

- Requires internet connection
- Rate limits may apply due to API restrictions

## Credits

Built by [Dr. Rogue]