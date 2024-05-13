# CHANGELOG

## [0.5.0] - 2024.5.14
### Fixed
- Handle exception that occurs when backend init trys to find a deleted chat

### Changed
- When downloding history, now the backend will store all messages in memory and write them to index at once, to avoid blocking regular update
- Use PDM package manager

## [0.4.0] - 2023.2.2

### Added
- Add nix flake deployment
- `no_redis` frontend config

### Changed
- `\clear` will do nothing, `\clear all` will clear all

### Fixed
- Error when proxy config is missed
- Improper call to msg.edit with `/refresh_chat_names`

## [0.3.1] - 2022.4.6

### Changed
- Ignore irrevalent requests when frontend bot in group
- Add '/search' for searching in a group

### Fixed
- Privacy whitelist considers only chat id, not peer id
- Wrong config path in docker-compose example
- Respond to own message in group
- Downloading messages in reversed order, causing remaining_msg count incorrect

## [0.3.0] - 2022.2.12

### Added
- User can refer to a chat by its name
- Display the newest message in status text
- Reply friendly err message when chat is not found, or no chat is specified
- `/refresh_chat_names` command

### Changed
- **[Breaking]** Separate session configuration to a standalone section
- Store all chat names on `start()`
- Show session name in status text

### Fixed
- New coming message handled by their original id instead of share id
- Exception when MessageDeleted carries no chat id
- Inconsistency in README

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
