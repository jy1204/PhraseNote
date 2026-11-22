<template>
  <div class="vrules-page">
    <div class="vrules-panel">
      <!-- 左侧圆形导航轨：按「基础/元音/辅音/组合/词尾」分组，每组带两字标题；点击/滚动均高亮 -->
      <div class="vrules-type">
        <div class="track-group" v-for="grp in groupedItems" :key="grp.label">
          <div class="track-group-title" v-if="grp.label">{{ grp.label }}</div>
          <ol>
            <li v-for="item in grp.items" :key="item.id" :data-id="item.id"
                :class="{ active: item.id === activeId, 'vrules-empty-anchor': item.empty }"
                @click="goAnchor(item.id)" :title="item.label">
              {{ item.short }}
            </li>
          </ol>
        </div>
      </div>

      <!-- 右侧内容区：搜索框 + 滚动卡片列表 -->
      <div class="vrules-knowledge">
        <div class="vrules-search pn-search">
          <el-input ref="searchInput" v-model="search" class="search-input"
                    placeholder="搜索字母" :prefix-icon="Search" />
          <button type="button" class="search-input-clear" @click="onClear"
                  title="清空搜索">
            <el-icon><CircleCloseFilled /></el-icon>
          </button>
        </div>
        <div class="vrules-scroll" ref="scrollRef" @scroll.passive="onScroll">
    <!-- ============ 章节 1：语音基础 ============ -->
    <section class="chapter" id="sec-basic" v-show="isVisible('#')" :class="{'vrules-hl': activeId === 'sec-basic'}">
    <div class="chapter-head"><span class="idx">1</span><h2>语音基础</h2><span class="cnt">2 个主题</span></div>
    <!-- ===== 主题 1：重读和轻读（对比关系 → 逐行对照表） ===== -->
    <div class="topic">
    <div class="topic-hd">
    <span class="tno">1</span><h3>重读和轻读</h3><span class="kind">对比</span>
    </div>
    <div class="box">
    <div class="box-hd"><span class="dot"></span>逐项对照</div>
    <div class="box-bd">
    <table class="ct">
    <thead>
    <tr><th class="k">对比项</th><th class="strong">重读音节</th><th class="weak">轻度音节</th></tr>
    </thead>
    <tbody>
    <tr><th class="k">音调</th><td class="strong">高</td><td class="weak">低</td></tr>
    <tr><th class="k">音量</th><td class="strong">大</td><td class="weak">小</td></tr>
    <tr><th class="k">发音</th><td class="strong">清楚</td><td class="weak">含糊</td></tr>
    <tr><th class="k">时间</th><td class="strong">长</td><td class="weak">短</td></tr>
    <tr><th class="k">符号</th><td class="strong"><span class="mark">'</span></td><td class="weak">—</td></tr>
    </tbody>
    </table>
    <div class="ct-eg">
    <div class="cap">重读例词</div>
    <span class="w">name<i>/neɪm/</i></span>
    <span class="w">happy<i>/ˈhæpi/</i></span>
    <span class="w">control<i>/kənˈtrəʊl/</i></span>
    <span class="w">contest<i>/ˈkɒntest/</i></span>
    </div>
    </div>
    </div>
    </div>
    <!-- ===== 主题 2：开音节与闭音节（分类关系 → 定义 + 分类树） ===== -->
    <div class="topic">
    <div class="topic-hd">
    <span class="tno">2</span><h3>开音节与闭音节</h3><span class="kind">分类</span>
    </div>
    <div class="box">
    <div class="box-hd"><span class="dot"></span>定义与分类</div>
    <div class="box-bd">
    <div class="defrow">
    <span class="n">1</span><b>开音节</b><span class="t">分为绝对开音节和相对开音节</span>
    </div>
    <div class="tree">
    <div class="syl">
    <div class="syl-hd"><span class="tag">A</span><h4>绝对开音节</h4></div>
    <div class="formula"><span class="unit u-v">元音</span><span class="op">结尾</span></div>
    <p>单个发音的元音字母结尾</p>
    <div class="egs"><span class="lead">eg.</span><span class="w">no</span><span class="w">she</span><span class="w">me</span></div>
    </div>
    <div class="syl">
    <div class="syl-hd"><span class="tag">B</span><h4>相对开音节</h4></div>
    <div class="formula">
    <span class="unit u-c">辅音</span><span class="op">+</span>
    <span class="unit u-v">元音</span><span class="op">+</span>
    <span class="unit u-c">辅音</span><span class="op">+</span>
    <span class="unit u-s">e</span>
    </div>
    <p>辅音 + 元音 + 辅音 + e（e 不发音）</p>
    <div class="egs"><span class="lead">eg.</span><span class="w">name</span><span class="w">bike</span><span class="w">kite</span></div>
    </div>
    </div>
    <div class="defrow">
    <span class="n">2</span><b>闭音节</b><span class="t">辅音字母（r 除外）结尾</span>
    </div>
    <div class="tree">
    <div class="syl">
    <div class="syl-hd"><span class="tag">A</span><h4>辅音结尾</h4></div>
    <div class="formula"><span class="unit u-v">元音</span><span class="op">+</span><span class="unit u-c">辅音</span><span class="op">结尾</span></div>
    <p>以一个或几个辅音字母（r 除外）结尾的音节叫闭音节，其中的元音字母读短音</p>
    <div class="egs"><span class="lead">eg.</span><span class="w">sit</span><span class="w">film</span><span class="w">desk</span></div>
    </div>
    </div>
    </div>
    </div>
    </div>
    </section>
    <!-- ============ 章节 2：元音字母 ============ -->

    <!-- ============ 章节 3：辅音字母 ============ -->

    <!-- ============ 章节 4：字母组合 ============ -->

    <!-- ============ 章节 5：词尾规则 ============ -->

    <div class="group-head" v-show="!search"><span class="idx">2</span><h2>元音字母</h2><span class="cnt">5 个字母</span></div>
    <section class="ltr" id="ltr-a" v-show="isVisible('a')" :class="{'vrules-hl': activeId === 'ltr-a'}">
    <div class="ltr-head">
    <span class="ltr-badge">a</span>
    <span class="cnt">6 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-long">/eɪ/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在重读开音节中读 /eɪ/</div><div class="r-eg"><span class="lead">eg.</span><span class="w">plane</span><span class="w">radio</span><span class="w">wake</span></div>
    <div class="except"><span class="badge">特例</span><p><b>have</b> 读作 <span class="ipa">/hæv/</span>，而不是 <s>/heɪ/</s></p></div></div></div>
    <div class="rule"><div class="r-main"><div class="r-cond">在 -ange 组合中读 /eɪ/</div><div class="r-eg"><span class="lead">eg.</span><span class="w">change</span><span class="w">strange</span><span class="w">arrange</span></div>
    <div class="except"><span class="badge">特例</span><p><b>orange</b> 读作 <span class="ipa">/ˈɒrɪndʒ/</span>，而不是 <s>/ˈeɪreɪndʒ/</s></p></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-short">/æ/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在重读音节前的闭音节中一般读 /æ/，有时也读 /ə/</div><div class="r-eg"><span class="w">activity<i>/ækˈtɪvəti/</i></span><span class="w">transcription<i>/trænˈskrɪpʃn/</i></span><span class="w">accept<i>/əkˈsept/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-weak">/ə/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在非重读音节中一般读 /ə/</div><div class="r-eg"><span class="w">substance<i>/ˈsʌbstəns/</i></span><span class="w">breakfast<i>/ˈbrekfəst/</i></span><span class="w">woman<i>/ˈwʊmən/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-short">/ɒ/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在 wh 后面读 /ɒ/</div><div class="r-eg"><span class="w">what<i>/wɒt/</i></span><span class="w">whatever<i>/wɒtˈevə/</i></span></div>
    <div class="except"><span class="badge">特例</span><p><b>whale</b> 读作 <span class="ipa">/weɪl/</span>，按重读开音节发音</p></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-back">/ɔː/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在 w 后面读 /ɔː/</div><div class="r-eg"><span class="w">water<i>/ˈwɔːtə/</i></span><span class="w">watch<i>/wɒtʃ/</i></span><span class="w">wander<i>/ˈwɒndə/</i></span></div>
    <div class="except"><span class="badge">特例</span><p><b>wave、wake</b> 读作 <span class="ipa">/weɪv/、/weɪk/</span>，按重读开音节发音</p></div></div></div>
    <div class="rule"><div class="r-main"><div class="r-cond">在 l 前面读 /ɔː/</div><div class="r-eg"><span class="w">chalk<i>/tʃɔːk/</i></span><span class="w">tall<i>/tɔːl/</i></span><span class="w">always<i>/ˈɔːlweɪz/</i></span><span class="w">talk<i>/tɔːk/</i></span></div>
    <div class="except"><span class="badge">特例</span><p><b>shall /ʃæl/ · valley · shallow · wallet</b> 例外</p></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-back">/ɑː/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在 s 之前读 /ɑː/</div><div class="r-eg"><span class="lead">eg.</span><span class="w">glass</span><span class="w">class</span><span class="w">grass</span><span class="w">fast</span></div>
    <div class="except"><span class="badge">特例</span><p><b>haste</b> 读 <span class="ipa">/heɪst/</span>，<b>taste</b> 读 <span class="ipa">/teɪst/</span>，<b>passage</b> 读 <span class="ipa">/ˈpæsɪdʒ/</span></p></div></div></div>
    </div></div>
    </div></section><section class="ltr" id="ltr-e" v-show="isVisible('e')" :class="{'vrules-hl': activeId === 'ltr-e'}">
    <div class="ltr-head">
    <span class="ltr-badge">e</span>
    <span class="cnt">3 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-long">/iː/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在重读开音节中读 /iː/</div><div class="r-eg"><span class="lead">eg.</span><span class="w">these<i>/ðiːz/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-short">/e/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在重读闭音节中读 /e/</div><div class="r-eg"><span class="w">egg<i>/eɡ/</i></span><span class="w">well<i>/wel/</i></span><span class="w">red<i>/red/</i></span><span class="w">hen<i>/hen/</i></span><span class="w">bell<i>/bel/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-silent">不发音</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">词尾或音节末尾时不发音</div><div class="r-eg"><span class="w">cape<i>/keɪp/</i></span><span class="w">rose<i>/rəʊz/</i></span><span class="w">bite<i>/baɪt/</i></span><span class="w">tube<i>/tjuːb/</i></span></div></div></div>
    </div></div>
    </div></section><section class="ltr" id="ltr-i" v-show="isVisible('i')" :class="{'vrules-hl': activeId === 'ltr-i'}">
    <div class="ltr-head">
    <span class="ltr-badge">i</span>
    <span class="cnt">2 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-long">/aɪ/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在重读开音节中读 /aɪ/</div><div class="r-eg"><span class="w">bike<i>/baɪk/</i></span><span class="w">crime<i>/kraɪm/</i></span><span class="w">knife<i>/naɪf/</i></span></div></div></div>
    <div class="rule"><div class="r-main"><div class="r-cond">在 -igh、-ight、-ign、-ild、-ind 中读 /aɪ/</div><div class="r-eg"><span class="w">bright<i>/braɪt/</i></span><span class="w">high<i>/haɪ/</i></span><span class="w">child<i>/tʃaɪld/</i></span></div>
    <div class="except"><span class="badge">特例</span><p><b>children /ˈtʃɪldrən/</b>、<b>wind /wɪnd/</b></p></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-short">/ɪ/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在重读闭音节中读 /ɪ/</div><div class="r-eg"><span class="w">big<i>/bɪɡ/</i></span><span class="w">city<i>/ˈsɪti/</i></span><span class="w">dish<i>/dɪʃ/</i></span><span class="w">kid<i>/kɪd/</i></span></div></div></div>
    </div></div>
    </div></section><section class="ltr" id="ltr-o" v-show="isVisible('o')" :class="{'vrules-hl': activeId === 'ltr-o'}">
    <div class="ltr-head">
    <span class="ltr-badge">o</span>
    <span class="cnt">3 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-long">/əʊ/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在重读开音节中读 /əʊ/</div><div class="r-eg"><span class="w">nose<i>/nəʊz/</i></span><span class="w">vote<i>/vəʊt/</i></span><span class="w">close<i>/kləʊz/</i></span></div></div></div>
    <div class="rule"><div class="r-main"><div class="r-cond">在双音节和多音节词中读 /əʊ/</div><div class="r-eg"><span class="w">alone<i>/əˈləʊn/</i></span><span class="w">local<i>/ˈləʊkl/</i></span></div></div></div>
    <div class="rule"><div class="r-main"><div class="r-cond">在词尾的 o 读 /əʊ/</div><div class="r-eg"><span class="w">also<i>/ˈɔːlsəʊ/</i></span><span class="w">piano<i>/piˈænəʊ/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-back">/uː/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在 move 和 prove 及其派生词中读 /uː/</div><div class="r-eg"><span class="w">move<i>/muːv/</i></span><span class="w">approve<i>/əˈpruːv/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-short">/ʌ/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">有时候读 /ʌ/</div><div class="r-eg"><span class="w">love</span><span class="w">another</span><span class="w">honey</span></div></div></div>
    </div></div>
    </div></section><section class="ltr" id="ltr-u" v-show="isVisible('u')" :class="{'vrules-hl': activeId === 'ltr-u'}">
    <div class="ltr-head">
    <span class="ltr-badge">u</span>
    <span class="cnt">4 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-long">/juː/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在重读开音节中读 /juː/</div><div class="r-eg"><span class="w">use<i>/juːz/</i></span><span class="w">student<i>/ˈstjuːdnt/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-back">/uː/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在字母 l、r、j 后读 /uː/</div><div class="r-eg"><span class="w">blue<i>/bluː/</i></span><span class="w">rule<i>/ruːl/</i></span><span class="w">June<i>/dʒuːn/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-short">/ʌ/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在重读闭音节中读 /ʌ/</div><div class="r-eg"><span class="w">bus<i>/bʌs/</i></span><span class="w">but<i>/bʌt/</i></span><span class="w">cup<i>/kʌp/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-back">/ʊ/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">还可以读作 /ʊ/</div><div class="r-eg"><span class="w">put<i>/pʊt/</i></span><span class="w">full<i>/fʊl/</i></span></div></div></div>
    </div></div>
    </div></section><div class="group-head" v-show="!search"><span class="idx">3</span><h2>辅音字母</h2><span class="cnt">9 个字母</span></div>
    <section class="ltr" id="ltr-b" v-show="isVisible('b')" :class="{'vrules-hl': activeId === 'ltr-b'}">
    <div class="ltr-head">
    <span class="ltr-badge">b</span>
    <span class="cnt">2 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-voice">/b/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">一般情况下读 /b/</div><div class="r-eg"><span class="w">book<i>/bʊk/</i></span><span class="w">box<i>/bɒks/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-silent">不发音</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在字母 m 后不发音</div><div class="r-eg"><span class="w">climb<i>/klaɪm/</i></span><span class="w">comb<i>/kəʊm/</i></span></div></div></div>
    <div class="rule"><div class="r-main"><div class="r-cond">在字母 t 前不发音</div><div class="r-eg"><span class="w">debt<i>/det/</i></span><span class="w">doubt<i>/daʊt/</i></span></div></div></div>
    </div></div>
    </div></section><section class="ltr" id="ltr-c" v-show="isVisible('c')" :class="{'vrules-hl': activeId === 'ltr-c'}">
    <div class="ltr-head">
    <span class="ltr-badge">c</span>
    <span class="cnt">4 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-clear">/k/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在元音字母 a、o、u 或辅音字母 l、r、t 之前读 /k/</div><div class="r-eg"><span class="w">cat</span><span class="w">cold</span><span class="w">cut</span><span class="w">clean</span><span class="w">cry</span><span class="w">fact</span></div></div></div>
    <div class="rule"><div class="r-main"><div class="r-cond">在词尾往往读 /k/</div><div class="r-eg"><span class="w">music<i>/ˈmjuːzɪk/</i></span><span class="w">logic<i>/ˈlɒdʒɪk/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-clear">/s/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在字母 e、i、y 之前读 /s/</div><div class="r-eg"><span class="w">nice</span><span class="w">city</span><span class="w">bicycle<i>/ˈbaɪsɪkl/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-clear">/ʃ/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在轻读音节中，ce / ci + 元音字母读 /ʃ/</div><div class="r-eg"><span class="w">ocean<i>/ˈəʊʃn/</i></span><span class="w">social<i>/ˈsəʊʃl/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-silent">不发音</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在辅音字母 s 后，c 不发音</div><div class="r-eg"><span class="w">scene<i>/siːn/</i></span><span class="w">science<i>/ˈsaɪəns/</i></span></div></div></div>
    </div></div>
    </div></section>    <section class="ltr" id="ltr-g" v-show="isVisible('g')" :class="{'vrules-hl': activeId === 'ltr-g'}">
    <div class="ltr-head">
    <span class="ltr-badge">g</span>
    <span class="cnt">3 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-voice">/g/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在元音字母 a、o、u 或辅音字母 l、r 之前读 /g/</div><div class="r-eg"><span class="w">gas</span><span class="w">god</span><span class="w">gum</span><span class="w">glad</span><span class="w">green</span></div></div></div>
    <div class="rule"><div class="r-main"><div class="r-cond">在元音字母 e、i 前少数读 /g/</div><div class="r-eg"><span class="w">get</span><span class="w">give</span></div></div></div>
    <div class="rule"><div class="r-main"><div class="r-cond">在词尾一般读 /g/</div><div class="r-eg"><span class="w">big</span><span class="w">leg</span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-voice">/dʒ/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">元音 e、i、y 前，一般读 /dʒ/</div><div class="r-eg"><span class="w">gentle<i>/ˈdʒentl/</i></span><span class="w">giraffe<i>/dʒəˈrɑːf/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-silent">不发音</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在辅音字母 n 前不发音</div><div class="r-eg"><span class="w">sign<i>/saɪn/</i></span><span class="w">design<i>/dɪˈzaɪn/</i></span></div></div></div>
    </div></div>
    </div></section><section class="ltr" id="ltr-h" v-show="isVisible('h')" :class="{'vrules-hl': activeId === 'ltr-h'}">
    <div class="ltr-head">
    <span class="ltr-badge">h</span>
    <span class="cnt">2 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-clear">/h/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">一般情况下读 /h/</div><div class="r-eg"><span class="w">hot</span><span class="w">house</span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-silent">不发音</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在元音 o 前不发音</div><div class="r-eg"><span class="w">hour</span><span class="w">honest</span></div></div></div>
    <div class="rule"><div class="r-main"><div class="r-cond">在字母 r、g 或组合 ex 后不发音</div><div class="r-eg"><span class="w">rhyme<i>/raɪm/</i></span><span class="w">ghost<i>/ɡəʊst/</i></span><span class="w">exhibition<i>/ˌeksɪˈbɪʃn/</i></span></div></div></div>
    </div></div>
    </div></section><section class="ltr" id="ltr-k" v-show="isVisible('k')" :class="{'vrules-hl': activeId === 'ltr-k'}">
    <div class="ltr-head">
    <span class="ltr-badge">k</span>
    <span class="cnt">2 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-clear">/k/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">一般情况下读 /k/</div><div class="r-eg"><span class="w">cake</span><span class="w">kite</span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-silent">不发音</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在字母 n 前不发音</div><div class="r-eg"><span class="w">knife<i>/naɪf/</i></span><span class="w">know<i>/nəʊ/</i></span></div></div></div>
    </div></div>
    </div></section><section class="ltr" id="ltr-n" v-show="isVisible('n')" :class="{'vrules-hl': activeId === 'ltr-n'}">
    <div class="ltr-head">
    <span class="ltr-badge">n</span>
    <span class="cnt">3 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-voice">/n/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">一般情况下读 /n/</div><div class="r-eg"><span class="w">line<i>/laɪn/</i></span><span class="w">snake<i>/sneɪk/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-back">/ŋ/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在辅音字母 k、g 或 [k] 之前读 <span class="ipa">/ŋ/</span></div><div class="r-eg"><span class="w">finger<i>/ˈfɪŋɡə/</i></span><span class="w">uncle<i>/ˈʌŋkl/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-silent">不发音</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在词尾 -mn 中，n 不发音</div><div class="r-eg"><span class="w">autumn<i>/ˈɔːtəm/</i></span></div></div></div>
    </div></div>
    </div></section><section class="ltr" id="ltr-s" v-show="isVisible('s')" :class="{'vrules-hl': activeId === 'ltr-s'}">
    <div class="ltr-head">
    <span class="ltr-badge">s</span>
    <span class="cnt">5 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-clear">/s/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在词首读 /s/</div><div class="r-eg"><span class="w">sit</span><span class="w">soap</span></div></div></div>
    <div class="rule"><div class="r-main"><div class="r-cond">在清辅音前读 /s/</div><div class="r-eg"><span class="w">desk</span><span class="w">newspaper</span></div></div></div>
    <div class="rule"><div class="r-main"><div class="r-cond">在元音字母和不发音的 e 之间，且为名词或形容词时，读 /s/</div><div class="r-eg"><span class="w">house<i>/haʊs/</i></span><span class="w">case<i>/keɪs/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-voice">/z/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在浊辅音前读 /z/</div><div class="r-eg"><span class="w">husband<i>/ˈhʌzbənd/</i></span></div></div></div>
    <div class="rule"><div class="r-main"><div class="r-cond">在元音字母和不发音的 e 之间，且为动词时，读 /z/</div><div class="r-eg"><span class="w">close<i>/kləʊz/</i></span><span class="w">raise<i>/reɪz/</i></span></div></div></div>
    <div class="rule"><div class="r-main"><div class="r-cond">在两个元音之间一般读 /z/</div><div class="r-eg"><span class="w">season<i>/ˈsiːzn/</i></span><span class="w">prison<i>/ˈprɪzn/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-clear">/ʃ/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在轻读音节中，在 -ia 或 -io 之前读 /ʃ/</div><div class="r-eg"><span class="w">Russian<i>/ˈrʌʃn/</i></span><span class="w">discussion<i>/dɪsˈkʌʃn/</i></span></div>
    <div class="except"><span class="badge">特例</span><p><b>decision</b> 读作 <span class="ipa">/dɪˈsɪʒn/</span></p></div></div></div>
    <div class="rule"><div class="r-main"><div class="r-cond">在轻读音节的词尾 -sure 中读 <span class="ipa">/ʃ/</span></div><div class="r-eg"><span class="w">pressure<i>/ˈpreʃə/</i></span></div>
    <div class="except"><span class="badge">特例</span><p><b>-sure</b> 前是元音时读 <span class="ipa">/ʒ/</span>，如 <b>pleasure</b> <span class="ipa">/ˈpleʒə/</span></p></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-silent">不发音</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">有时不发音</div><div class="r-eg"><span class="w">island<i>/ˈaɪlənd/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-long">-s</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">动词或名词词尾加 s 时，s 的发音分三种情况：</div>
    <div class="triple">
    <div class="t"><b>/s/</b>词尾是清辅音 + s<small>eg. lamps, likes</small></div>
    <div class="t"><b>/z/</b>词尾是浊辅音或元音 + s<small>eg. plays, pigs</small></div>
    <div class="t"><b>/ɪz/</b>词尾是 /s/ /z/ /ʃ/ /ʒ/ /tʃ/ /dʒ/ 后，在词尾 e 后加 s<small>eg. pages, glasses, washes, catches</small></div>
    </div>
    </div></div>
    </div></div>
    </div></section><section class="ltr" id="ltr-t" v-show="isVisible('t')" :class="{'vrules-hl': activeId === 'ltr-t'}">
    <div class="ltr-head">
    <span class="ltr-badge">t</span>
    <span class="cnt">3 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-clear">/t/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">一般情况读 /t/</div><div class="r-eg"><span class="w">tea</span><span class="w">test</span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-clear">/ʃ/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在轻读音节中，在 -ia、-ie、-io 之前读 /ʃ/</div><div class="r-eg"><span class="w">patient<i>/ˈpeɪʃnt/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-silent">不发音</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在词尾 -sten、-stle、-ften 中，t 不发音</div><div class="r-eg"><span class="w">listen</span><span class="w">often</span></div></div></div>
    </div></div>
    </div></section><section class="ltr" id="ltr-w" v-show="isVisible('w')" :class="{'vrules-hl': activeId === 'ltr-w'}">
    <div class="ltr-head">
    <span class="ltr-badge">w</span>
    <span class="cnt">2 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-clear">/w/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">一般情况读 /w/</div><div class="r-eg"><span class="w">wide<i>/waɪd/</i></span><span class="w">sweet<i>/swiːt/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-silent">不发音</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在字母 r 前或 s 后不发音</div><div class="r-eg"><span class="w">write<i>/raɪt/</i></span><span class="w">answer<i>/ˈɑːnsə/</i></span></div></div></div>
    </div></div>
    </div></section><div class="group-head" v-show="!search"><span class="idx">4</span><h2>字母组合</h2><span class="cnt">16 个组合</span></div>
    <section class="ltr" id="ltr-ar" v-show="isVisible('ar')" :class="{'vrules-hl': activeId === 'ltr-ar'}">
    <div class="ltr-head">
    <span class="ltr-badge">ar</span>
    <span class="cnt">3 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-back">/ɑː/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">一般情况下读 /ɑː/</div><div class="r-eg"><span class="w">start<i>/stɑːt/</i></span><span class="w">dark<i>/dɑːk/</i></span><span class="w">smart<i>/smɑːt/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-weak">/ə/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在非重读音节读 /ə/</div><div class="r-eg"><span class="w">grammar<i>/ˈɡræmə/</i></span><span class="w">popular<i>/ˈpɒpjələ/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-back">/ɔː/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在字母 w、qu 之后读 /ɔː/</div><div class="r-eg"><span class="w">warm<i>/wɔːm/</i></span><span class="w">quarter<i>/ˈkwɔːtə/</i></span></div></div></div>
    </div></div>
    </div></section>
    <section class="ltr" id="ltr-ch" v-show="isVisible('ch')" :class="{'vrules-hl': activeId === 'ltr-ch'}">
    <div class="ltr-head">
    <span class="ltr-badge">ch</span>
    <span class="cnt">3 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-clear">/tʃ/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">一般读 /tʃ/</div><div class="r-eg"><span class="w">speech<i>/spiːtʃ/</i></span><span class="w">choose<i>/tʃuːz/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-clear">/k/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">也读 /k/</div><div class="r-eg"><span class="w">school<i>/skuːl/</i></span><span class="w">stomach<i>/ˈstʌmək/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-clear">/ʃ/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">偶尔读 /ʃ/</div><div class="r-eg"><span class="w">brochure<i>/ˈbrəʊʃə/</i></span><span class="w">machine<i>/məˈʃiːn/</i></span></div></div></div>
    </div></div>
    </div></section>
    <section class="ltr" id="ltr-ea" v-show="isVisible('ea')" :class="{'vrules-hl': activeId === 'ltr-ea'}">
    <div class="ltr-head">
    <span class="ltr-badge">ea</span>
    <span class="cnt">4 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-long">/iː/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">读 /iː/</div><div class="r-eg"><span class="w">heat<i>/hiːt/</i></span><span class="w">teach<i>/tiːtʃ/</i></span><span class="w">easy<i>/ˈiːzi/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-short">/e/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">读 /e/</div><div class="r-eg"><span class="w">pleasure<i>/ˈpleʒə/</i></span><span class="w">instead<i>/ɪnˈsted/</i></span><span class="w">bread<i>/bred/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-weak">/ɪə/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">读 /ɪə/</div><div class="r-eg"><span class="w">theatre<i>/ˈθɪətə/</i></span><span class="w">idea<i>/aɪˈdɪə/</i></span><span class="w">really<i>/ˈrɪəli/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-long">/eɪ/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">少数读 /eɪ/</div><div class="r-eg"><span class="w">great<i>/ɡreɪt/</i></span><span class="w">break<i>/breɪk/</i></span></div></div></div>
    </div></div>
    <div class="confuse">
    <div class="confuse-hd"><span class="badge">易混</span><span class="t">同一组合，读音不同，注意区分</span></div>
    <div class="confuse-bd">
    <div class="cf-row">
    <span class="cf-side"><b>mean</b><span class="ipa">/miːn/</span></span>
    <span class="cf-vs">·</span>
    <span class="cf-side"><b>meant</b><span class="ipa">/ment/</span></span>
    <span class="cf-vs">·</span>
    <span class="cf-side"><b>meaning</b><span class="ipa">/ˈmiːnɪŋ/</span></span>
    </div>
    <div class="cf-row">
    <span class="cf-side"><b>breath</b><span class="ipa">/breθ/</span></span>
    <span class="cf-vs">vs</span>
    <span class="cf-side"><b>breathe</b><span class="ipa">/briːð/</span></span>
    </div>
    <div class="cf-row">
    <span class="cf-side"><b>please</b><span class="ipa">/pliːz/</span></span>
    <span class="cf-vs">vs</span>
    <span class="cf-side"><b>pleasure</b><span class="ipa">/ˈpleʒə/</span></span>
    </div>
    </div></div>
    </div></section>
    <section class="ltr" id="ltr-ear" v-show="isVisible('ear')" :class="{'vrules-hl': activeId === 'ltr-ear'}">
    <div class="ltr-head">
    <span class="ltr-badge">ear</span>
    <span class="cnt">4 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-long">/ɪə/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">读 /ɪə/</div><div class="r-eg"><span class="w">ear<i>/ɪə/</i></span><span class="w">hear<i>/hɪə/</i></span><span class="w">clear<i>/klɪə/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-short">/eə/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">读 /eə/</div><div class="r-eg"><span class="w">wear<i>/weə/</i></span><span class="w">bear<i>/beə/</i></span><span class="w">swear<i>/sweə/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-back">/ɜː/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">读 /ɜː/</div><div class="r-eg"><span class="w">earth<i>/ɜːθ/</i></span><span class="w">early<i>/ˈɜːli/</i></span><span class="w">learn<i>/lɜːn/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-back">/ɑː/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">读 /ɑː/</div><div class="r-eg"><span class="w">heart<i>/hɑːt/</i></span></div></div></div>
    </div></div>
    </div></section>
    <section class="ltr" id="ltr-ed" v-show="isVisible('ed')" :class="{'vrules-hl': activeId === 'ltr-ed'}">
    <div class="ltr-head">
    <span class="ltr-badge">ed</span>
    <span class="cnt">3 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-clear">/t/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在轻辅音后读 /t/</div><div class="r-eg"><span class="w">passed<i>/pɑːst/</i></span><span class="w">helped<i>/helpt/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-voice">/d/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在元音和浊辅音后读 /d/</div><div class="r-eg"><span class="w">opened<i>/ˈəʊpənd/</i></span><span class="w">show<i>/ʃəʊd/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-short">/ɪd/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在 t、d 之后读 /ɪd/</div><div class="r-eg"><span class="w">needed<i>/ˈniːdɪd/</i></span><span class="w">wanted<i>/ˈwɒntɪd/</i></span></div></div></div>
    </div></div>
    </div></section>
    <section class="ltr" id="ltr-ere" v-show="isVisible('ere')" :class="{'vrules-hl': activeId === 'ltr-ere'}">
    <div class="ltr-head">
    <span class="ltr-badge">ere</span>
    <span class="cnt">3 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-long">/ɪə/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">读 /ɪə/</div><div class="r-eg"><span class="w">here<i>/hɪə/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-short">/eə/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">读 /eə/</div><div class="r-eg"><span class="w">there<i>/ðeə/</i></span><span class="w">where<i>/weə/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-back">/ɜː/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">读 /ɜː/</div><div class="r-eg"><span class="w">were<i>/wɜː/</i></span></div></div></div>
    </div></div>
    </div></section>
    <section class="ltr" id="ltr-ex" v-show="isVisible('ex')" :class="{'vrules-hl': activeId === 'ltr-ex'}">
    <div class="ltr-head">
    <span class="ltr-badge">ex</span>
    <span class="cnt">3 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-voice">/ɪɡz/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">ex + 元音，且重音不在第一个音节上读 /ɪɡz/</div><div class="r-eg"><span class="w">exam<i>/ɪɡˈzæm/</i></span><span class="w">example<i>/ɪɡˈzɑːmpəl/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-clear">/ɪks/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">ex + 辅音，且重音不在第一个音节上读 /ɪks/</div><div class="r-eg"><span class="w">excuse<i>/ɪkˈskjuːs/</i></span><span class="w">expect<i>/ɪkˈspekt/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-clear">/eks/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">ex 后不管跟什么字母，只要重音落在第一个音节上读 /eks/</div><div class="r-eg"><span class="w">exercise<i>/ˈeksəsaɪz/</i></span><span class="w">expert<i>/ˈekspɜːt/</i></span></div></div></div>
    </div></div>
    </div></section>
    <section class="ltr" id="ltr-gh" v-show="isVisible('gh')" :class="{'vrules-hl': activeId === 'ltr-gh'}">
    <div class="ltr-head">
    <span class="ltr-badge">gh</span>
    <span class="cnt">3 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-voice">/g/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在词首读 /g/</div><div class="r-eg"><span class="w">ghost<i>/ɡəʊst/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-clear">/f/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">词尾为字母组合 ough 时一般读 /f/</div><div class="r-eg"><span class="w">enough<i>/ɪˈnʌf/</i></span><span class="w">cough<i>/kɒf/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-silent">不发音</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在元音字母 i 后不发音（igh 组合）</div><div class="r-eg"><span class="w">light</span><span class="w">bright</span><span class="w">night</span></div></div></div>
    </div></div>
    </div></section>
    <section class="ltr" id="ltr-oo" v-show="isVisible('oo')" :class="{'vrules-hl': activeId === 'ltr-oo'}">
    <div class="ltr-head">
    <span class="ltr-badge">oo</span>
    <span class="cnt">3 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-back">/uː/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">d、k 之前读 /uː/，food / mood / noodle 需长读</div><div class="r-eg"><span class="w">food<i>/fuːd/</i></span><span class="w">mood<i>/muːd/</i></span><span class="w">noodle<i>/ˈnuːdl/</i></span></div></div></div>
    <div class="rule"><div class="r-main"><div class="r-cond">复合名词需短读，其余多数读 /uː/</div><div class="r-eg"><span class="w">broom<i>/bruːm/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-short">/ʊ/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">t 前读 /uː/，foot 一词需短读 /ʊ/</div><div class="r-eg"><span class="w">foot<i>/fʊt/</i></span></div></div></div>
    <div class="rule"><div class="r-main"><div class="r-cond">复合名词需短读</div><div class="r-eg"><span class="w">football<i>/ˈfʊtbɔːl/</i></span><span class="w">bedroom<i>/ˈbedrʊm/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-short">/ʌ/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">l 和 d 之间读 /ʌ/</div><div class="r-eg"><span class="w">flood<i>/flʌd/</i></span><span class="w">blood<i>/blʌd/</i></span></div></div></div>
    </div></div>
    </div></section>
    <section class="ltr" id="ltr-or" v-show="isVisible('or')" :class="{'vrules-hl': activeId === 'ltr-or'}">
    <div class="ltr-head">
    <span class="ltr-badge">or</span>
    <span class="cnt">3 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-back">/ɔː/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在重读音节中读 /ɔː/</div><div class="r-eg"><span class="w">forty<i>/ˈfɔːti/</i></span><span class="w">morning<i>/ˈmɔːnɪŋ/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-back">/ɜː/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在 w 后读 /ɜː/</div><div class="r-eg"><span class="w">word<i>/wɜːd/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-back">/ə/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在词尾非重读时读 /ə/</div><div class="r-eg"><span class="w">doctor<i>/ˈdɒktə/</i></span></div></div></div>
    </div></div>
    </div></section>
    <section class="ltr" id="ltr-our" v-show="isVisible('our')" :class="{'vrules-hl': activeId === 'ltr-our'}">
    <div class="ltr-head">
    <span class="ltr-badge">our</span>
    <span class="cnt">3 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-back">/ɔː/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在重读音节中读 /ɔː/</div><div class="r-eg"><span class="w">course<i>/kɔːs/</i></span><span class="w">four<i>/fɔː/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-long">/aʊə/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">词首且重读时读 /aʊə/</div><div class="r-eg"><span class="w">our<i>/ˈaʊə/</i></span><span class="w">hour<i>/ˈaʊə/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-back">/ɜː/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在重读音节中读 /ɜː/</div><div class="r-eg"><span class="w">journey<i>/ˈdʒɜːni/</i></span></div></div></div>
    </div></div>
    </div></section>
    <section class="ltr" id="ltr-ow" v-show="isVisible('ow')" :class="{'vrules-hl': activeId === 'ltr-ow'}">
    <div class="ltr-head">
    <span class="ltr-badge">ow</span>
    <span class="cnt">2 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-long">/aʊ/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">读 /aʊ/</div><div class="r-eg"><span class="w">how<i>/haʊ/</i></span><span class="w">allow<i>/əˈlaʊ/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-long">/əʊ/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">读 /əʊ/</div><div class="r-eg"><span class="w">snow<i>/snəʊ/</i></span><span class="w">follow<i>/ˈfɒləʊ/</i></span></div></div></div>
    </div></div>
    </div></section>
    <section class="ltr" id="ltr-sion" v-show="isVisible('sion')" :class="{'vrules-hl': activeId === 'ltr-sion'}">
    <div class="ltr-head">
    <span class="ltr-badge">sion</span>
    <span class="cnt">2 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-voice">/ʒn/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在元音后读 /ʒn/</div><div class="r-eg"><span class="w">television<i>/ˈtelɪvɪʒn/</i></span><span class="w">decision<i>/dɪˈsɪʒn/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-clear">/ʃn/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在辅音后读 /ʃn/</div><div class="r-eg"><span class="w">expression<i>/ɪkˈspreʃn/</i></span><span class="w">impression<i>/ɪmˈpreʃn/</i></span></div></div></div>
    </div></div>
    </div></section>
    <section class="ltr" id="ltr-th" v-show="isVisible('th')" :class="{'vrules-hl': activeId === 'ltr-th'}">
    <div class="ltr-head">
    <span class="ltr-badge">th</span>
    <span class="cnt">2 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-clear">/θ/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">清辅音 /θ/</div><div class="r-eg"><span class="w">health<i>/helθ/</i></span><span class="w">method<i>/ˈmeθəd/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-voice">/ð/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">浊辅音 /ð/</div><div class="r-eg"><span class="w">weather<i>/ˈweðə/</i></span><span class="w">with<i>/wɪð/</i></span></div></div></div>
    </div></div>
    <div class="confuse">
    <div class="confuse-hd"><span class="badge">易混发音</span><span class="t">注意 th 的清浊与词形变化</span></div>
    <div class="confuse-bd">
    <div class="cf-row">
    <span class="cf-side"><b>cloth</b><span class="ipa">/θ/</span></span>
    <span class="cf-vs">·</span>
    <span class="cf-side"><b>clothes</b><span class="ipa">/ð/</span></span>
    <span class="cf-vs">·</span>
    <span class="cf-side"><b>clothing</b><span class="ipa">/ð/</span></span>
    </div>
    <div class="cf-row">
    <span class="cf-side"><b>breath</b><span class="ipa">/θ/</span></span>
    <span class="cf-vs">vs</span>
    <span class="cf-side"><b>breathe</b><span class="ipa">/ð/</span></span>
    </div>
    <div class="cf-row">
    <span class="cf-side"><b>south</b><span class="ipa">/θ/</span></span>
    <span class="cf-vs">vs</span>
    <span class="cf-side"><b>southern</b><span class="ipa">/ð/</span></span>
    </div>
    <div class="cf-row">
    <span class="cf-side"><b>though</b><span class="ipa">/ð/</span></span>
    <span class="cf-vs">·</span>
    <span class="cf-side"><b>although</b><span class="ipa">/ð/</span></span>
    <span class="cf-vs">vs</span>
    <span class="cf-side"><b>thought</b><span class="ipa">/θ/</span></span>
    </div>
    <div class="cf-row">
    <span class="cf-side"><b>bath</b><span class="ipa">/θ/</span></span>
    <span class="cf-vs">vs</span>
    <span class="cf-side"><b>bathe</b><span class="ipa">/ð/</span></span>
    </div>
    <div class="cf-row">
    <span class="cf-side"><b>worth</b><span class="ipa">/θ/</span></span>
    <span class="cf-vs">vs</span>
    <span class="cf-side"><b>worthy</b><span class="ipa">/ð/</span></span>
    </div>
    </div></div>
    </div></section>
    <section class="ltr" id="ltr-tion" v-show="isVisible('tion')" :class="{'vrules-hl': activeId === 'ltr-tion'}">
    <div class="ltr-head">
    <span class="ltr-badge">tion</span>
    <span class="cnt">2 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-clear">/ʃn/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">一般读 /ʃn/</div><div class="r-eg"><span class="w">nation<i>/ˈneɪʃn/</i></span><span class="w">station<i>/ˈsteɪʃn/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-voice">/tʃn/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">偶尔读 /tʃn/</div><div class="r-eg"><span class="w">question<i>/ˈkwestʃn/</i></span><span class="w">suggestion<i>/səˈdʒestʃn/</i></span></div></div></div>
    </div></div>
    </div></section>
    <section class="ltr" id="ltr-wh" v-show="isVisible('wh')" :class="{'vrules-hl': activeId === 'ltr-wh'}">
    <div class="ltr-head">
    <span class="ltr-badge">wh</span>
    <span class="cnt">2 类读音</span>
    </div><div class="ltr-body">
    <div class="snd-group">
    <div class="snd-label ph-clear">/w/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">一般情况读 /w/</div><div class="r-eg"><span class="w">what<i>/wɒt/</i></span><span class="w">where<i>/weə/</i></span><span class="w">why<i>/waɪ/</i></span></div></div></div>
    </div></div>
    <div class="snd-group">
    <div class="snd-label ph-clear">/h/</div>
    <div class="snd-items">
    <div class="rule"><div class="r-main"><div class="r-cond">在字母 o 前读 /h/</div><div class="r-eg"><span class="w">who<i>/huː/</i></span><span class="w">whole<i>/həʊl/</i></span><span class="w">whose<i>/huːz/</i></span></div></div></div>
    </div></div>
    </div></section>

        <div v-if="noMatch" class="vrules-empty">未找到匹配的字母</div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
  // 发音规则页逻辑：圆形导航、字母搜索、滚动高亮（见 js/VRulesPage.js）
  import { ref, nextTick } from 'vue'
  import { Search, CircleCloseFilled } from '@element-plus/icons-vue'
  import { useVRulesPage } from '../js/VRulesPage.js'
  import '../css/public/VRulesPage.css'

  const {
    search, activeId, scrollRef, visibleItems, groupedItems, noMatch,
    isVisible, goAnchor, onScroll, clearSearch,
  } = useVRulesPage()

  // 清空搜索后把焦点还给输入框，方便接着输入
  const searchInput = ref(null)
  function onClear() {
    clearSearch()
    nextTick(() => searchInput.value?.focus?.())
  }
</script>
