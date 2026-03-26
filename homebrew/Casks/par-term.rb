cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.30.0"
  sha256 arm:   "5b31b65946aee732764e4fd68dcdc7c2c80ab07da523d12fdb261781bd6d9529",
         intel: "1cca2d1f93c4ff020905fc13d596f5bdc5eeea18b440186cc31d07dd6c7e0dbe"

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
