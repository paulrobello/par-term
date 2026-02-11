cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.13.0"
  sha256 arm:   "5d9292538c77fd0253d8d7dfe9cb28172e3b352f374c3311e567d400ed501803",
         intel: "8870779e0a8ef570650d49c76a13016d6b89bd7644289f2a67804fb1e3bd13ce"

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
