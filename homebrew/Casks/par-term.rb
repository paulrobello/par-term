cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.31.0"
  sha256 arm:   "9db9e6e6d1361fd8b84299508af2176213344ffbaaedb3bdcaeeae3e2c52acf6",
         intel: "b16a97fb5696e64d161a288fc5ee5df3e0be078aaf914a541bba3e1871cea197"

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
