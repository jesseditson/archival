# frozen_string_literal: true

require 'tags/asset'

FIXTURES_DIR = File.join(File.dirname(__FILE__),
                         '..', 'fixtures')

RSpec.describe Asset do
  before(:all) do
    @cwd = FIXTURES_DIR
    Liquid::Template.file_system = Liquid::LocalFileSystem.new(
      @cwd
    )
    Liquid::Template.error_mode = :strict
    Liquid::Template.register_tag('asset', Asset)
    Asset.root_dir = @cwd
  end

  context 'parsing an asset tag' do
    it "Doesn't work if template_path isn't set" do
      content = Liquid::Template.parse("{% asset '../foo/bar.thing' %}")
      out = content.render
      expect(out).to eq(
        'Liquid error: template_path must be provided to parse when using assets'
      )
    end

    it 'preserves paths from the root' do
      asset_path = 'foo/bar.thing'
      content = Liquid::Template.parse("{% asset '#{asset_path}' %}")
      out = content.render('template_path' => File.join(@cwd,
                                                        'template-name.liquid'))
      expect(out).to eq asset_path
    end
    it 'rewrites paths from a child dir' do
      asset_path = 'foo/bar.thing'
      content = Liquid::Template.parse("{% asset '#{asset_path}' %}")
      out = content.render('template_path' => File.join(@cwd, 'subdir',
                                                        'template-name.liquid'))
      expect(out).to eq "../#{asset_path}"
    end
  end
end

class AssetError < Liquid::Error
end
