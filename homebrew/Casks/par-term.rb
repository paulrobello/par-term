cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.50.0"
  sha256 arm:   "454227501a818835c0bbc3208b349ddad0ad31162d82fbee612ce23bf9a2f2ad",
         intel: "69d6943ca7f7b3a0ee5adb20790de3f50c97963a9524c1296d0745105d051038"

  url "https://github.com/paulrobello/par-term/releases/download/v#{version}/par-term-macos-#{arch}.zip"
  name "par-term"
  desc "Cross-platform GPU-accelerated terminal emulator with inline graphics support"
  homepage "https://github.com/paulrobello/par-term"

  depends_on macos: ">= :catalina"

  livecheck do
    url :homepage
    strategy :github_latest
  end

  app "par-term.app"

  zap trash: [
    "~/Library/Application Support/par-term",
    "~/Library/Preferences/com.paulrobello.par-term.plist",
    "~/Library/Saved Application State/com.paulrobello.par-term.savedState",
    "~/.config/par-term",
  ]
end
