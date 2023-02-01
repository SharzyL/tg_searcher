set -xe

version=$(git tag --points-at=HEAD | head -1)
[ "${version:0:1}" = "v" ]
version=${version:1}
grep -q "$version" __version__
grep -q "\[$version\]" CHANGELOG.md

