cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.30.1"
  sha256 arm:   "0a68839d74e38e595c98972542fc77e528971e9b35d40cd1aad97d3c5d23184d",
         intel: "be475e520cff30cffa2f362784beb3f3a8473f7ffeb3afd71e963fe5ce725546"

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
