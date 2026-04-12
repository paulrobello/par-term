cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.30.5"
  sha256 arm:   "5b5ca75040049890cd7e6082fad135084cd1fa02306e978432e0b737f8e753e7",
         intel: "b0415540d9aadad3c23bc9ae1ea2b8aca3665ec352e08b43e8f7f00b43f73c74"

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
