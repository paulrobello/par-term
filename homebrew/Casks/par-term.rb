cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.37.1"
  sha256 arm:   "58b15dfdb980f85336afb24049bd88498cc98931fd429afebc9341d2b6874ef5",
         intel: "5b62b09b5a9b607e6ee6f3ca5e5ae7f8a9bba594ce56dbc08beae9d493312f83"

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
