cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.37.0"
  sha256 arm:   "c11fbc7c1bf4856730fe2c39c86fd6bae3c7e22492456f3afbbe41d76b6e4fe0",
         intel: "21e1c4f0907558839d63414fe00eb49620aa0bd816d5b85ed1eb1dc60a238790"

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
