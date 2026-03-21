cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.29.1"
  sha256 arm:   "b01d6f075e59782d0aa141e676c4dcf7e428e1f1d87b2cd8967ee284f7571e49",
         intel: "b539e5d5d3f246504d55b1fda79b75d28cc64c1f58a1c5b6ffeb25c5b6adc64e"

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
