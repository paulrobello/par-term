cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.44.0"
  sha256 arm:   "1cc58e5bd0c932015d9fd009ca166c03fe6f11d24b21a11e77fc1316d7495da0",
         intel: "eb05fb40b7875add5962af05820421a25f2e7268a3223ad4d5c2b0a9208f9199"

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
