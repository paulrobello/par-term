cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.30.6"
  sha256 arm:   "14caf452cbbf6a48d959aa022590a3d9454f8965ea745d5426e23e469f26817f",
         intel: "e20299339d1552adb08789384c18dc441778c1abfa120e64fa841c662a6dffaa"

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
