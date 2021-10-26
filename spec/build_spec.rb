require 'build'

FIXTURES_DIR = File.join(File.dirname(__FILE__), "fixtures")

def write_snapshot(name, content)
    File.open(File.join(FIXTURES_DIR, "snapshots", name), "w+") { |file|
        file.write(content)
    }
end
def snapshot(name)
    File.read(File.join(FIXTURES_DIR, "snapshots", name))
end

RSpec.describe Build do
    context "simple website" do
        before(:each) do
            root = File.join(FIXTURES_DIR, "simple_website")
            Layout.reset_cache()
            @build = Build.new("root" => root)
            Dir.chdir root
        end
        it "has the right pages" do
            expect(@build.page_templates.keys).to eq ["index"]
        end
        it "renders the index page" do
            out = @build.render("index")
            if ENV["WRITE_SNAPSHOT"]
                write_snapshot("simple_website_index", out)
            end
            expect(out).to eq snapshot("simple_website_index")
        end
    end
end
