cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.52.0"
  sha256 arm:   "26911235118b4385ae4dc00f8869f25d79f37159ea2c30f7f8820dca0d50a5f8",
         intel: "9af6fbf526f2df87119426e12d6eba72540637ff588466c76d048b49bc5fffa3"

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
