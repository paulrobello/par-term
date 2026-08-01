cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.41.0"
  sha256 arm:   "ce7e07e583d5a99d4f145db716a344bd5964171344b69d5fdb69befe18c7c130",
         intel: "339dc73bde17c5700f059a6636cc383ff5a67f418a2428a6deb9647245bd8fa6"

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
