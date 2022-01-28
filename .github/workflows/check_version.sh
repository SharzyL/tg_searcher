set -xe

version=$(git tag --points-at=HEAD | head -1)
[ "${version:0:1}" = "v" ]
version=${version:1}
grep -q "__version__.*'$version'" setup.py
grep -q "\[$version\]" CHANGELOG.md

