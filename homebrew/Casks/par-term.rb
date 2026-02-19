cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.19.0"
  sha256 arm:   "b14f83541facac45dde759d4a5e37384e17739727a223976af24443ca15ee2ef",
         intel: "240349545ea768e5a1bf8f35be6d9c8006e747392d5dcaa86b10b7e338341223"

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
