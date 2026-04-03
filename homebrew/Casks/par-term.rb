cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.30.4"
  sha256 arm:   "148e650d7246489bd751dd5e56450311d484f76209224ee8418c900a89f94cda",
         intel: "5eb5d7792b6163411840c5849b66a05c99e998df732dcc6275e95ccc8734cc03"

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
