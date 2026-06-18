cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.33.1"
  sha256 arm:   "ddd8f57370ddd3a3e4ab42648d244818df76c2be8229acf2c530556f2a162770",
         intel: "055e05d773acd3e82f1e1ccd8d58e233dcc7440693173e89aedef28bf7d6240d"

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
