cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.30.11"
  sha256 arm:   "c49694b0c96397d6cbbb9ea958942b6d3f5cf38f74b29245bb2f7bd28938dd52",
         intel: "5ae03b3e9c5594ce589d62ab48a0e666cd91403739d7795ed937cbd91e6ae8cc"

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
