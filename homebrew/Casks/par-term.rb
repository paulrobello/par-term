cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.29.0"
  sha256 arm:   "09e58f57c44cf03f84671ac0922f3332d655f1c176207170c5d961917509453a",
         intel: "1fed6b2fd17b211b34e5362440d80ce3c4298f275736b13b81b5cd7d77cc183b"

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
