cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.25.0"
  sha256 arm:   "123903f52bc414ff6a72f8b3deafc9ce86fb0fdb3f0c0518122e3af5399a6cc8",
         intel: "cc7254cd98f80cd7aa08e55f530379d2f3edbe569cd09c232f3513f7f7a5eefa"

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
