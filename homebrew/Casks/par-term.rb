cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.6.0"
  sha256 arm:   "8243953706a2a40c4b643c6f0833119cc8870669f2683d92da3a6be7f71b6dc8",
         intel: "409d28dfdf8bcfe752303f7c921a1309616a277fe46ff744e572b2e6e3a702e6"

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
