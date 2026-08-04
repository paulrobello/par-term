cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.42.0"
  sha256 arm:   "b1ba2b758449293934a4e19646ce62cb505ae442082d9c554ce236fafdea01fa",
         intel: "3836cc048c442a4d0d68e316454fec29681645c88c02b96dad438fb21638ea8a"

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
