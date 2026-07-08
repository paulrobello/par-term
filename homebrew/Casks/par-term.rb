cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.35.1"
  sha256 arm:   "fc70a8a22faaf58b9af55076f32e304f2cffabc232d109d61035f95f1fe55dbd",
         intel: "d8b247362651a309b33fb6711f23b8fe5b33204d5e644dcc086adefa72cce2d4"

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
