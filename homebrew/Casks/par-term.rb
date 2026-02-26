cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.23.0"
  sha256 arm:   "ae451be2aa1a7713134da39e50a6526e75538c6282623823c57c6aea8c655395",
         intel: "26b5601f7cbd4024a5df5014a5f88507e66aeb0d519c4e7eeff799335edcdd7b"

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
