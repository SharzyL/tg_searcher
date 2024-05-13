set -xe

version=$(git tag --points-at=HEAD | head -1)
[ "${version:0:1}" = "v" ]
version=${version:1}
grep -q "$version" tg_searcher/__init__.py
grep -q "\[$version\]" CHANGELOG.md

