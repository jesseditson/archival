# frozen_string_literal: true

require 'tags/layout'
require 'tmpdir'
require 'fileutils'

theme_content = "
THEME_FILE {{ name }}
{{page_content}}
END_THEME
"
partial_content = "
PARTIAL_CONTENT
{{ name }}
"
basic_content = "
THEME_FILE 

END_THEME
"
string_var_content = "
THEME_FILE foo

END_THEME
"
var_var_content = "
THEME_FILE foo_var

END_THEME
"
wrapped_content = "
THEME_FILE 
 page content
END_THEME
"
wrapped_var_template = "
{% assign name = 'name var' %}
{% layout 'theme', name = {{ name }}%}
Here is some page content for {{ name }}
"
wrapped_var_content = "


THEME_FILE name var

Here is some page content for name var

END_THEME
"
partial_template = "
{% assign name = 'partialpage' %}
{% layout 'theme', name = {{ name }}%}
The partial is here:
{% include 'partial' %}
and this is below it
"
partial_render_content = "


THEME_FILE partialpage

The partial is here:

PARTIAL_CONTENT
partialpage

and this is below it

END_THEME
"

RSpec.describe Layout do
  before(:all) do
    @cwd = Dir.mktmpdir
    Dir.chdir @cwd
    Dir.mkdir('layout')
    File.open('layout/theme.liquid',
              'w+') do |file|
      file.write(theme_content)
    end
    File.open('_partial.liquid', 'w+') do |file|
      file.write(partial_content)
    end
    Liquid::Template.error_mode = :strict
    Liquid::Template.register_tag('layout',
                                  Layout)
    Layout.reset_cache
    Liquid::Template.file_system = Liquid::LocalFileSystem.new(@cwd)
  end

  after(:all) do
    # puts "tmpdir: #{@cwd}"
    # FileUtils.rm_r(@cwd)
  end

  context 'loading layouts' do
    it "loads layouts from a relative 'layout' dir" do
      content = Liquid::Template.parse("{% layout 'theme' %}")
      out = content.render
      expect(out).to eq basic_content
    end
    it "errors when layout name doesn't exist" do
      content = Liquid::Template.parse("{% layout 'teme' %}")
      out = content.render
      expect(out).to eq 'Liquid error: No layouts named teme found.'
    end
    it 'errors when syntax is wrong' do
      content = Liquid::Template.parse('{% layout teme pork %}')
      out = content.render
      expect(out).to eq 'Liquid error: Bad layout name argument'
    end
  end
  context 'variables' do
    it 'accepts variable arguments that get rendered' do
      content = Liquid::Template.parse("{% layout 'theme', name: 'foo' %}")
      out = content.render
      expect(out).to eq string_var_content
    end
    it 'renders variable arguments that are variables' do
      content = Liquid::Template.parse(
        "{% assign foo = 'foo_var' %}{% layout 'theme', name: {{foo}} %}"
      )
      out = content.render
      expect(out).to eq var_var_content
    end
    it 'renders variable arguments that are passed in' do
      content = Liquid::Template.parse("{% layout 'theme', name: {{foo}} %}")
      out = content.render('foo' => 'test')
      expect(out).to eq "
THEME_FILE test

END_THEME
"
    end
  end
  context 'wrapping & API' do
    it 'wraps other content inside the layout' do
      content = Liquid::Template.parse("{% layout 'theme' %} page content")
      out = content.render
      expect(out).to eq wrapped_content
    end
    it 'properly renders variables in the inner content' do
      content = Liquid::Template.parse(wrapped_var_template)
      out = content.render
      expect(out).to eq wrapped_var_content
    end
    it 'properly renders partials in the inner content' do
      content = Liquid::Template.parse(partial_template)
      out = content.render
      expect(out).to eq partial_render_content
    end
  end
end
