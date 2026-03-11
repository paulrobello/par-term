cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.26.0"
  sha256 arm:   "17e65f911228a502708cae03c76c71734ae58f3bea5dcedd9b887f6059250a23",
         intel: "cb68177a2a0563991e7ee5c21144448afbd1624695ed3b88c7752a8ad45fc876"

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
