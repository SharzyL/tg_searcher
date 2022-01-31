# CHANGELOG

## [0.2.0] - 2022.1.31

### Added
- `monitor_all` (and `excluded_chats`) backend configuration
- Pypi auto upload workflow
- (Partial) nix flake support
- Redis alive check on frontend startup

### Changed
- **[Breaking]** Index schema upgraded, new field "sender" is added, user should re-build the database
- New redis data key protocol to avoid key conflict between frontends
- Cache name of all dialogs for faster `find_chat_id`

### Fixed
- Too long message when `/stat`
- English prompt message on `/download_chat`
- Key error on empty config
- Key error on MessageEdit event in unindexed chat

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
