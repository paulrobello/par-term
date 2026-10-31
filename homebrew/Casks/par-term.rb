cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.51.0"
  sha256 arm:   "86ce6d74328819e08d2f7494d5423617233a4523510ee20b63e8f4579af84405",
         intel: "9c1c0154529b3a5cd14116778d5f854ae045aa50f09bbc5fa9e63a2264d92cd3"

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
