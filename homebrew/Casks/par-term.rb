cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.45.2"
  sha256 arm:   "8e8960a866630770bfe1c11ff60f2cc7b223b2f21cc42fc5a505192d788f0e0e",
         intel: "3ac02c155b3b61a65254d8cf3a518c6dd3a2114c30ce16599a805446db005367"

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
