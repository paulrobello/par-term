cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.15.0"
  sha256 arm:   "e1f99b03e2a154ea221ebb8bac1e3647f24446b412a57b8cbc0583a64d7ae763",
         intel: "2db4f1c039a0e1a225ebe08406b36aef19e3c40ea73c747f7536e2ada68a4f72"

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
