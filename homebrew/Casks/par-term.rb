cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.45.1"
  sha256 arm:   "7e00239a3560e645ffa3e07013e28c2258832d80c7da6e0b07a70318f50e55fc",
         intel: "ca23d445cec47ef6ab0569983fbdc88aa468e8786b275b8bb5a576cdde52dda0"

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
