cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.30.3"
  sha256 arm:   "f46be9c4e11936d33a51586de197b31514cc4135d9632d791dced031a14c1a9f",
         intel: "426badce3098f96813d24e7818cacd2b67ce493ad540a2e2ef103701fb3e81f7"

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
