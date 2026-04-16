cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.30.8"
  sha256 arm:   "ad0e2b9864540ff0bbc91393e7afcddc77fdfbe18e9cb4d8a1b799781e1da63b",
         intel: "eafc423288fe6ebd2efbb946dbdf18a458f46857c4de2eb5524f0ec917f9c4bb"

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
