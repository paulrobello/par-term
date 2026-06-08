cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.32.3"
  sha256 arm:   "c66de51f93bf7c41746983fc71a974d9012f93be266f8e7d416fb2de1ba8b123",
         intel: "ba984f709f7416de3ff53c6576fe72e2fce1c9c6dd852411de9d10e69e3589d0"

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
