cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.30.2"
  sha256 arm:   "b5f6fade9e18f775584dce1fbc2859cf8ba103789d52477c78a793feed04f035",
         intel: "8ceff578659bd2900322973154813c21bbbb2c9152d2e6c7241f06f49e20fb5e"

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
