cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.30.12"
  sha256 arm:   "d5a0c0352e35648fdaa445cdbfe94e282dab8387619489a49da8579dd956cdf0",
         intel: "8fddb2402e690d371ecdd5cfc3a080dc4eca490853ae56571dc7eebad91c1347"

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
