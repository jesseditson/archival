# frozen_string_literal: true

require 'archival'

FIXTURES_DIR = File.join(File.dirname(__FILE__),
                         '..', 'fixtures')

def write_snapshot(name, content)
  File.open(
    File.join(FIXTURES_DIR, 'snapshots',
              name), 'w+'
  ) do |file|
    file.write(content)
  end
end

def snapshot(name)
  File.read(File.join(FIXTURES_DIR, 'snapshots',
                      name))
end

RSpec.describe Archival::Builder do
  context 'sort_objects' do
    before(:each) do
      root = File.join(FIXTURES_DIR,
                       'simple_website')
      Layout.reset_cache
      config = Archival::Config.new('root' => root)
      @builder = Archival::Builder.new(config)
      Dir.chdir root
    end
    it 'sort_objects with order' do
      objects = {}
      objects['last'] = { 'bar' => 'baz', 'order' => 5 }
      objects['first'] = { 'buzz' => 'baz', 'order' => 1 }
      sorted = @builder.sort_objects(objects)
      expect(sorted.length).to eq(2)
      expect(sorted[0]).to eq(objects['first'])
      expect(sorted[1]).to eq(objects['last'])
      expect(sorted['first']).to eq(objects['first'])
    end
    it 'sort_objects by name' do
      objects = {}
      objects['2'] = { 'bar' => 'baz' }
      objects['1'] = { 'buzz' => 'baz' }
      sorted = @builder.sort_objects(objects)
      expect(sorted.length).to eq(2)
      expect(sorted[0]).to eq(objects['1'])
      expect(sorted[1]).to eq(objects['2'])
    end
  end
  context 'simple website' do
    before(:each) do
      root = File.join(FIXTURES_DIR,
                       'simple_website')
      Layout.reset_cache
      config = Archival::Config.new('root' => root)
      @builder = Archival::Builder.new(config)
      Dir.chdir root
    end
    it 'has the right pages' do
      expect(@builder.page_templates.keys).to eq ['index']
    end
    it 'renders the index page' do
      out = @builder.render('index')
      if ENV['WRITE_SNAPSHOT']
        write_snapshot('simple_website_index',
                       out)
      end
      expect(out).to eq snapshot('simple_website_index')
    end
  end
  context 'dynamic pages' do
    before(:all) do
      @root = File.join(FIXTURES_DIR,
                        'dynamic_pages')
      Layout.reset_cache
      config = Archival::Config.new('root' => @root)
      @builder = Archival::Builder.new(config)
      Dir.chdir @root
      FileUtils.rm_rf(File.join(@root, 'dist'))
      @builder.write_all
    end
    it 'renders one page per item' do
      posts_dir = File.join(@root, 'dist', 'post')
      expect(File.exist?(posts_dir)).to be(true)
      posts = Dir.entries(posts_dir).reject { |f| f.start_with? '.' }
      expect(posts.length).to be(2)
    end
    it 'does not render the template' do
      template_path = File.join(@root, 'dist', 'post.html')
      expect(File.exist?(template_path)).to be(false)
    end
    it 'has the correct content' do
      post_file = File.join(@root, 'dist', 'post', 'another-post.html')
      expect(File.exist?(post_file)).to be(true)
      content = File.read post_file
      if ENV['WRITE_SNAPSHOT']
        write_snapshot('dynamic_pages_post',
                       content)
      end
      expect(content).to eq snapshot('dynamic_pages_post')
    end
  end
  context 'path rewriting' do
    before(:all) do
      @root = File.join(FIXTURES_DIR,
                        'path_rewriting')
      Layout.reset_cache
      config = Archival::Config.new('root' => @root)
      @builder = Archival::Builder.new(config)
      Dir.chdir @root
      FileUtils.rm_rf(File.join(@root, 'dist'))
      @builder.write_all
    end
    it 'retains paths in dist' do
      index_path = File.join(@root, 'dist', 'index.html')
      index_content = File.read(index_path)
      expect(index_content).to include('src="../img/bob.jpeg')
      expect(index_content).to include('href="../style/theme.css"')
    end
    it 'rewrites paths in dist dynamic subfolders' do
      post_path = File.join(@root, 'dist', 'post', 'another-post.html')
      post_content = File.read(post_path)
      expect(post_content).to include('src="../../img/bob.jpeg"')
    end
    it 'rewrites paths in dist dynamic subfolders in markdown' do
      post_path = File.join(@root, 'dist', 'post', 'another-post.html')
      post_content = File.read(post_path)
      expect(post_content).to include('href="../../img/foo.jpeg"')
    end
    it 'rewrites paths to other dynamic objects' do
      post_path = File.join(@root, 'dist', 'post', 'another-post.html')
      post_content = File.read(post_path)
      expect(post_content).to include('href="some-post-name.html"')
    end
    it 'rewrites paths in layouts when used in subfolders' do
      post_path = File.join(@root, 'dist', 'post', 'another-post.html')
      post_content = File.read(post_path)
      expect(post_content).to include('href="../../style/theme.css"')
    end
  end
end
