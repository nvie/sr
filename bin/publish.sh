#!/bin/sh
set -e

GITHUB_USER="nvie"
GITHUB_REPO="sr"

NAME=$(cat Cargo.toml | grep -Ee '^name[[:space:]]*=' | cut -d'"' -f2)
VERSION=$(cat Cargo.toml | grep -Ee '^version[[:space:]]*=' | cut -d'"' -f2)
TARBALL="$NAME-$VERSION-mac.tar.gz"

cargo build --release
( cd target/release && tar -czf "$TARBALL" "$NAME" )

URL="https://github.com/$GITHUB_USER/$GITHUB_REPO/releases/download/v$VERSION/$TARBALL"
SHA=$(shasum -a 256 "target/release/$TARBALL")

open -R "target/release/$TARBALL"

echo "Tarball created successfully."
echo "Press [Return] to open GitHub so you can manually create a new release and upload your tarball."
read
open "https://github.com/$GITHUB_USER/$GITHUB_REPO/releases/new?title=v$VERSION&tag=v$VERSION"
read
echo "Paste the following lines in your Homebrew tap":
echo ""
echo "  url \"$URL\""
echo "  sha256 \"$SHA\""
echo "  version \"$VERSION\""
echo ""
