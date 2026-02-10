cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.12.0"
  sha256 arm:   "db42c5c4a09a07ebdedac24bb244a26a5fa4947a4e0696ad4178ae87e4134336",
         intel: "49e3983f3e6af4fbafca66982920525ff74a30cd608efbee01aad96a89b7e9ee"

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
