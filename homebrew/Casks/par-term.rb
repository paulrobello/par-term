cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.32.0"
  sha256 arm:   "35ba65b01c4dfb7d38b8c3d448c2bdbb0cabc603315e071ff474633450a3fd2f",
         intel: "78fb22f87ab86e116c32d4729f03def88a51581cb7ba6bc16c4a697b51da57e7"

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
