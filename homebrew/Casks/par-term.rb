cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.16.0"
  sha256 arm:   "74824c2a3f7000aec52253405e8f576fd9c7fd7fe28a17981fee861c87fef774",
         intel: "04581a405d7d3ba0f95141897559c24724def8ad2099002827eee59dd2574e7b"

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
