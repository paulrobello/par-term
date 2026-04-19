cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.30.9"
  sha256 arm:   "e5a92f1b8e3ecc0ec533e3dc947be7006881c4421c71611b5cb7f022dc08fc80",
         intel: "6ab7aa711a0a6acbba4fe55ab0b34421e42c0a78fbcbf6fac75e869e6257923a"

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
