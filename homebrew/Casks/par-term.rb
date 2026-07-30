cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.38.0"
  sha256 arm:   "6b2a36f66b6eb93bdee4ead79c7964c5305acde3337a9ee3f317f0bf47f9c47e",
         intel: "c5e9720191829249885406ffe300451543e86e7799ce5278272b8f4d3cdae938"

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
