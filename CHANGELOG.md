# CHANGELOG

## [0.1.2] - 2022.1.28

### Added
- Bot frontend: `/track_chat` command for admin

### Changed
- All file moved to Unix linebreak
- More detailed log
- Correct command documentation

### Removed
- Backend: `indexed_chats` configuration. User should directly add index via frontend
- `requirement.txt` for embracing python module

### Fixed
- Yet some `chat_id` type conversion
- `main()` call in `main.py`
- Incorrect command arg parse
- Non-working docker build

## [0.1.1] - 2022.1.27
 
The first version that is deployed to PyPI
