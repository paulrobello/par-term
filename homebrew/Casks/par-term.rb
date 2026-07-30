cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.39.0"
  sha256 arm:   "dc9049efea088ab1e63d7a94ee9b3489e6c1df3fba9b5e3fa1e8504e287ae5a2",
         intel: "4944eabd36e65c20ef3ca5b62f33f3d96cb6dc88b809d27c8a96f43b67cd67e3"

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
