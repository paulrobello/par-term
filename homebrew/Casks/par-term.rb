cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.35.2"
  sha256 arm:   "ddb5c0f0b6b770511123e8aea193db8fcefdee0c659fd3313e832bacdf4d784a",
         intel: "bae9c33cd41831c946744696f283df29f80623ce70912990bded49b45ba674db"

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
