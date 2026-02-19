cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.18.0"
  sha256 arm:   "ae262c11a955eb60f8f8cf973df97ff169dad9c22d52680db49ec0823b0e5189",
         intel: "cf8331753bf984fc1441946f81a58a424deabb26c59531c228836a3999092ff4"

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
